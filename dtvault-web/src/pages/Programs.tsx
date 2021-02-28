import React from 'react';
import {
    Box,
    Breadcrumb,
    BreadcrumbItem,
    BreadcrumbLink,
    Center,
    CircularProgress,
    Container,
    SimpleGrid,
    Text,
    Flex,
} from '@chakra-ui/react';
import { ChevronRightIcon } from '@chakra-ui/icons';
import { Program, useProgramsQuery } from '../generated/graphql';
import { format, parseISO } from 'date-fns';

type ItemProps = {
    item: Program;
};

const Item: React.FC<ItemProps> = ({ item }) => (
    <Box borderWidth="1px" borderRadius="md" px="3" py="2">
        <Text>{item.name}</Text>
        <Flex justifyContent="space-between" mt="1">
            <Text color="gray.500" fontSize="sm">
                {format(parseISO(item.startAt), 'yyyy/M/d H:mm')}
            </Text>
            <Text color="gray.500" fontSize="sm">
                {item.service.name}
            </Text>
        </Flex>
    </Box>
);

function Programs() {
    const { loading, error, data } = useProgramsQuery();
    return (
        <Container maxW="container.lg" mt="1rem">
            <Breadcrumb spacing="8px" separator={<ChevronRightIcon color="gray.500" />} mb="1.25rem">
                <BreadcrumbItem isCurrentPage>
                    <BreadcrumbLink>番組一覧</BreadcrumbLink>
                </BreadcrumbItem>
            </Breadcrumb>
            {loading ? (
                <Center flexDirection="column">
                    <CircularProgress isIndeterminate color="blue.300" />
                    <Text mt="3">読み込み中……</Text>
                </Center>
            ) : error ? (
                <Text color="red.500">{JSON.stringify(error)}</Text>
            ) : (
                <SimpleGrid columns={3} spacing="20px">
                    {data?.programs?.map((item: any) => (
                        <Item item={item} />
                    ))}
                </SimpleGrid>
            )}
        </Container>
    );
}

export default Programs;
